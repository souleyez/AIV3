import { buildApiError } from './api-error.js';
import { readLocalSecretBindingIdsHeader } from './local-account-state.js';
import { readLocalThreadId } from './local-browser-state.js';

export const DEFAULT_FETCH_TIMEOUT_MS = 45000;

export function createFetchJsonClient(dependencies = {}) {
  const {
    fetchImpl = (...args) => globalThis.fetch(...args),
    readSecretBindingIdsHeader = readLocalSecretBindingIdsHeader,
    readLocalThreadId: readThreadId = readLocalThreadId,
    defaultTimeoutMs = DEFAULT_FETCH_TIMEOUT_MS,
  } = dependencies;

  return async function fetchJson(url, options = {}) {
    const {
      timeoutMs = defaultTimeoutMs,
      signal,
      ...fetchOptions
    } = options;
    const isFormData = typeof FormData !== 'undefined' && fetchOptions.body instanceof FormData;
    const secretBindingIds = readSecretBindingIdsHeader();
    const controller = !signal && timeoutMs > 0 && typeof AbortController !== 'undefined'
      ? new AbortController()
      : null;
    const timeoutId = controller
      ? setTimeout(() => controller.abort(), timeoutMs)
      : null;

    let response;
    try {
      response = await fetchImpl(url, {
        cache: 'no-store',
        credentials: 'include',
        ...fetchOptions,
        signal: signal || controller?.signal,
        headers: {
          Accept: 'application/json',
          ...(fetchOptions.body && !isFormData ? { 'Content-Type': 'application/json' } : {}),
          ...(secretBindingIds ? { 'X-AI-Data-Platform-Secret-Binding-Ids': secretBindingIds } : {}),
          'X-AI-Data-Platform-Local-Thread-Id': readThreadId(),
          ...(fetchOptions.headers || {}),
        },
        body: fetchOptions.body && !isFormData && typeof fetchOptions.body !== 'string'
          ? JSON.stringify(fetchOptions.body)
          : fetchOptions.body,
      });
    } catch (error) {
      if (error?.name === 'AbortError') {
        const seconds = Math.max(1, Math.ceil(timeoutMs / 1000));
        throw buildApiError(
          {
            error: 'request_timeout',
            message: `请求超时（${seconds} 秒）：${url}`,
          },
          `请求超时（${seconds} 秒）：${url}。请确认本地 API/数据库已启动，或稍后重试。`,
          408,
        );
      }
      throw error;
    } finally {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    }

    const contentType = response.headers.get('content-type') || '';
    const payload = contentType.includes('application/json')
      ? await response.json()
      : await response.text();

    if (!response.ok) {
      const message = typeof payload === 'string'
        ? payload
        : payload?.message || payload?.payload?.message || payload?.error || `Request failed: ${response.status}`;
      throw buildApiError(payload, message, response.status);
    }

    return payload;
  };
}

export const fetchJson = createFetchJsonClient();
