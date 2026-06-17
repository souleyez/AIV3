import {
  externalObservabilityProxyHeaderValue,
  hasExternalObservabilityAccessCookie,
} from './external-observability-access.js';

const DEFAULT_PLATFORM_API_BASE_URL = 'http://127.0.0.1:3000';

function normalizeBaseUrl() {
  const baseUrl = process.env.PLATFORM_API_BASE_URL || DEFAULT_PLATFORM_API_BASE_URL;
  return baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl;
}

export function buildPlatformApiUrl(pathname, search = '') {
  const normalizedPath = pathname.startsWith('/') ? pathname : `/${pathname}`;
  return `${normalizeBaseUrl()}${normalizedPath}${search}`;
}

export function isExternalObservabilityPath(pathSegments) {
  const path = Array.isArray(pathSegments) ? pathSegments.join('/') : '';
  return path === 'external/conversation-tests'
    || /^external\/conversation-tests\/[^/]+\/timeline$/.test(path)
    || path === 'external/integrations/channels'
    || /^external\/integrations\/[^/]+\/enable$/.test(path)
    || /^external\/integrations\/[^/]+\/disable$/.test(path)
    || /^external\/integrations\/[^/]+\/retry$/.test(path)
    || /^external\/integrations\/[^/]+\/rotate-secret$/.test(path)
    || /^external\/integrations\/[^/]+\/rotate-token$/.test(path)
    || /^external\/integrations\/[^/]+\/reply-dispatch$/.test(path);
}

export async function proxyPlatformApiRequest(request, pathSegments) {
  try {
    const isObservationPath = isExternalObservabilityPath(pathSegments);
    if (
      isObservationPath
      && !hasExternalObservabilityAccessCookie(request.headers.get('cookie'))
    ) {
      return Response.json(
        {
          error: 'external_observability_access_required',
          message: '外部集成观测需要访问密钥',
        },
        { status: 401 },
      );
    }

    const incomingUrl = new URL(request.url);
    const path = Array.isArray(pathSegments) ? pathSegments.join('/') : '';
    const targetUrl = buildPlatformApiUrl(`/v1/${path}`, incomingUrl.search);

    const headers = new Headers();
    const incomingContentType = request.headers.get('content-type');
    const accept = request.headers.get('accept');

    if (incomingContentType) headers.set('content-type', incomingContentType);
    if (accept) headers.set('accept', accept);
    const cookie = request.headers.get('cookie');
    if (cookie) {
      headers.set('cookie', cookie);
    }
    const secretBindingIds = request.headers.get('x-ai-data-platform-secret-binding-ids');
    if (secretBindingIds) {
      headers.set('x-ai-data-platform-secret-binding-ids', secretBindingIds);
    }
    const localThreadId = request.headers.get('x-ai-data-platform-local-thread-id');
    if (localThreadId) {
      headers.set('x-ai-data-platform-local-thread-id', localThreadId);
    }
    if (isObservationPath) {
      const observabilityKey = externalObservabilityProxyHeaderValue();
      if (observabilityKey) {
        headers.set('x-ai-data-platform-external-observability-key', observabilityKey);
      }
    }

    const response = await fetch(targetUrl, {
      method: request.method,
      headers,
      body: ['GET', 'HEAD'].includes(request.method) ? undefined : await request.text(),
      cache: 'no-store',
    });

    const forwardedHeaders = new Headers();
    ['content-type', 'cache-control', 'etag', 'content-disposition', 'content-security-policy', 'referrer-policy'].forEach((name) => {
      const value = response.headers.get(name);
      if (value) forwardedHeaders.set(name, value);
    });
    const contentType = response.headers.get('content-type') || '';
    if (!contentType.includes('text/event-stream')) {
      const contentLength = response.headers.get('content-length');
      if (contentLength) forwardedHeaders.set('content-length', contentLength);
    } else {
      forwardedHeaders.set('connection', 'keep-alive');
      forwardedHeaders.set('x-accel-buffering', 'no');
    }
    const setCookies = typeof response.headers.getSetCookie === 'function'
      ? response.headers.getSetCookie()
      : [response.headers.get('set-cookie')].filter(Boolean);
    setCookies.forEach((value) => forwardedHeaders.append('set-cookie', value));

    if (!forwardedHeaders.has('content-type')) {
      forwardedHeaders.set('content-type', 'application/json; charset=utf-8');
    }

    if (contentType.includes('text/event-stream')) {
      return new Response(response.body, {
        status: response.status,
        headers: forwardedHeaders,
      });
    }

    const payload = await response.arrayBuffer();
    return new Response(payload, {
      status: response.status,
      headers: forwardedHeaders,
    });
  } catch (error) {
    return Response.json(
      {
        error: 'platform_api_unreachable',
        message: error instanceof Error ? error.message : 'platform-api 不可用',
      },
      { status: 502 },
    );
  }
}
