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

export function parseSseEventBlock(block) {
  const event = { event: 'message', data: '' };
  const dataLines = [];
  String(block || '')
    .split(/\r?\n/)
    .forEach((line) => {
      if (!line || line.startsWith(':')) {
        return;
      }
      const separator = line.indexOf(':');
      const field = separator >= 0 ? line.slice(0, separator) : line;
      const rawValue = separator >= 0 ? line.slice(separator + 1) : '';
      const value = rawValue.startsWith(' ') ? rawValue.slice(1) : rawValue;
      if (field === 'event') {
        event.event = value || 'message';
      } else if (field === 'data') {
        dataLines.push(value);
      }
    });
  event.data = dataLines.join('\n');
  return event;
}

export function createFetchSseJsonClient(dependencies = {}) {
  const {
    fetchImpl = (...args) => globalThis.fetch(...args),
    readSecretBindingIdsHeader = readLocalSecretBindingIdsHeader,
    readLocalThreadId: readThreadId = readLocalThreadId,
  } = dependencies;

  return async function fetchSseJson(url, options = {}, handlers = {}) {
    const secretBindingIds = readSecretBindingIdsHeader();
    const response = await fetchImpl(url, {
      cache: 'no-store',
      credentials: 'include',
      ...options,
      headers: {
        Accept: 'text/event-stream',
        ...(options.body ? { 'Content-Type': 'application/json' } : {}),
        ...(secretBindingIds ? { 'X-AI-Data-Platform-Secret-Binding-Ids': secretBindingIds } : {}),
        'X-AI-Data-Platform-Local-Thread-Id': readThreadId(),
        ...(options.headers || {}),
      },
      body: options.body && typeof options.body !== 'string'
        ? JSON.stringify(options.body)
        : options.body,
    });

    if (!response.ok) {
      const contentType = response.headers.get('content-type') || '';
      const payload = contentType.includes('application/json')
        ? await response.json()
        : await response.text();
      const message = typeof payload === 'string'
        ? payload
        : payload?.message || payload?.payload?.message || payload?.error || `Request failed: ${response.status}`;
      throw buildApiError(payload, message, response.status);
    }
    if (!response.body) {
      throw new Error('当前浏览器不支持流式响应读取。');
    }

    const decoder = new TextDecoder();
    const reader = response.body.getReader();
    let buffer = '';
    let completedPayload = null;
    let terminalError = null;

    function handleBlock(block) {
      const parsed = parseSseEventBlock(block);
      if (!parsed.data) {
        return;
      }
      let payload = parsed.data;
      try {
        payload = JSON.parse(parsed.data);
      } catch {
        // Some SSE producers may send plain text; keep it readable.
      }
      handlers.onEvent?.(parsed.event, payload);
      if (parsed.event.endsWith('.delta')) {
        const delta = typeof payload === 'string' ? payload : payload?.delta || '';
        if (delta) {
          handlers.onDelta?.(delta, payload, parsed.event);
        }
      } else if (parsed.event.endsWith('.completed')) {
        completedPayload = payload?.response || payload?.data?.response || payload;
      } else if (parsed.event === 'error') {
        terminalError = payload;
      }
    }

    while (true) {
      const { value, done } = await reader.read();
      if (done) {
        break;
      }
      buffer += decoder.decode(value, { stream: true });
      const blocks = buffer.split(/\r?\n\r?\n/);
      buffer = blocks.pop() || '';
      blocks.forEach(handleBlock);
    }
    buffer += decoder.decode();
    if (buffer.trim()) {
      handleBlock(buffer);
    }

    if (terminalError) {
      const errorPayload = terminalError?.error || terminalError;
      throw buildApiError(
        errorPayload,
        errorPayload?.message || terminalError?.message || '流式请求失败',
        terminalError?.status || 500,
      );
    }
    return completedPayload;
  };
}

export const fetchSseJson = createFetchSseJsonClient();
