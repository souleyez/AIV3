import {
  externalObservabilityProxyHeaderValue,
  hasExternalObservabilityAccessCookie,
} from '../../../../../lib/external-observability-access';
import { buildPlatformApiUrl } from '../../../../../lib/platform-api';

export const dynamic = 'force-dynamic';

const DEFAULT_LIMIT = 200;
const MAX_LIMIT = 500;

function requestedLimit(request) {
  const raw = new URL(request.url).searchParams.get('limit');
  const parsed = Number(raw || DEFAULT_LIMIT);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return DEFAULT_LIMIT;
  }
  return Math.min(Math.floor(parsed), MAX_LIMIT);
}

function accessDenied() {
  return Response.json(
    {
      error: 'external_observability_access_required',
      message: 'Codex 执行器队列统计需要访问密钥',
    },
    { status: 401 },
  );
}

function proxyHeaders(request) {
  const headers = new Headers({ accept: 'application/json' });
  const cookie = request.headers.get('cookie');
  if (cookie) {
    headers.set('cookie', cookie);
  }
  const observabilityKey = externalObservabilityProxyHeaderValue();
  if (observabilityKey) {
    headers.set('x-ai-data-platform-external-observability-key', observabilityKey);
  }
  return headers;
}

export async function GET(request) {
  if (!hasExternalObservabilityAccessCookie(request.headers.get('cookie'))) {
    return accessDenied();
  }

  const limit = requestedLimit(request);
  const targetUrl = buildPlatformApiUrl(
    '/v1/workflow-tasks/queue-stats',
    `?limit=${limit}`,
  );
  const response = await fetch(targetUrl, {
    method: 'GET',
    headers: proxyHeaders(request),
    cache: 'no-store',
  });
  const payload = await response.json().catch(() => ({}));
  return Response.json(payload, { status: response.status });
}
