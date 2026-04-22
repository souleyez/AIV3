const DEFAULT_PLATFORM_API_BASE_URL = 'http://127.0.0.1:3000';

function normalizeBaseUrl() {
  const baseUrl = process.env.PLATFORM_API_BASE_URL || DEFAULT_PLATFORM_API_BASE_URL;
  return baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl;
}

export function buildPlatformApiUrl(pathname, search = '') {
  const normalizedPath = pathname.startsWith('/') ? pathname : `/${pathname}`;
  return `${normalizeBaseUrl()}${normalizedPath}${search}`;
}

export async function proxyPlatformApiRequest(request, pathSegments) {
  try {
    const incomingUrl = new URL(request.url);
    const path = Array.isArray(pathSegments) ? pathSegments.join('/') : '';
    const targetUrl = buildPlatformApiUrl(`/v1/${path}`, incomingUrl.search);

    const headers = new Headers();
    const contentType = request.headers.get('content-type');
    const accept = request.headers.get('accept');

    if (contentType) headers.set('content-type', contentType);
    if (accept) headers.set('accept', accept);

    const response = await fetch(targetUrl, {
      method: request.method,
      headers,
      body: ['GET', 'HEAD'].includes(request.method) ? undefined : await request.text(),
      cache: 'no-store',
    });

    const payload = await response.text();
    const forwardedHeaders = new Headers();
    ['content-type', 'cache-control', 'etag'].forEach((name) => {
      const value = response.headers.get(name);
      if (value) forwardedHeaders.set(name, value);
    });

    if (!forwardedHeaders.has('content-type')) {
      forwardedHeaders.set('content-type', 'application/json; charset=utf-8');
    }

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
