import {
  externalObservabilityProxyHeaderValue,
  hasExternalObservabilityAccessCookie,
} from '../../../../lib/external-observability-access';
import { buildPlatformApiUrl } from '../../../../lib/platform-api';

export const dynamic = 'force-dynamic';

const DEFAULT_LIMIT = 20;
const MAX_LIMIT = 50;

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
      message: 'Codex 执行器任务观测需要访问密钥',
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
    '/v1/workflow-executions',
    `?kind=codex_host_task_workflow&limit=${limit}`,
  );
  const response = await fetch(targetUrl, {
    method: 'GET',
    headers: proxyHeaders(request),
    cache: 'no-store',
  });
  const payload = await response.json().catch(() => []);
  if (!response.ok) {
    return Response.json(payload, { status: response.status });
  }
  return Response.json({
    tasks: Array.isArray(payload) ? payload : [],
  });
}
