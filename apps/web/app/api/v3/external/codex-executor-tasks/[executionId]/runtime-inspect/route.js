import {
  externalObservabilityProxyHeaderValue,
  hasExternalObservabilityAccessCookie,
} from '../../../../../../lib/external-observability-access';
import { buildPlatformApiUrl } from '../../../../../../lib/platform-api';

export const dynamic = 'force-dynamic';

function accessDenied() {
  return Response.json(
    {
      error: 'external_observability_access_required',
      message: 'Codex 执行器任务详情需要访问密钥',
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

async function fetchPlatformJson(url, request) {
  const response = await fetch(url, {
    method: 'GET',
    headers: proxyHeaders(request),
    cache: 'no-store',
  });
  const payload = await response.json().catch(() => ({}));
  return { response, payload };
}

export async function GET(request, context) {
  if (!hasExternalObservabilityAccessCookie(request.headers.get('cookie'))) {
    return accessDenied();
  }

  const params = await context.params;
  const executionId = String(params?.executionId || '').trim();
  if (!executionId) {
    return Response.json(
      {
        error: 'workflow_execution_id_required',
        message: '缺少 workflow execution id',
      },
      { status: 400 },
    );
  }

  const inspectUrl = buildPlatformApiUrl(
    `/v1/workflow-executions/${encodeURIComponent(executionId)}/runtime-inspect`,
  );
  const tasksUrl = buildPlatformApiUrl(
    `/v1/workflow-executions/${encodeURIComponent(executionId)}/tasks`,
  );
  const [{ response, payload }, tasksResult] = await Promise.all([
    fetchPlatformJson(inspectUrl, request),
    fetchPlatformJson(tasksUrl, request),
  ]);
  if (response.ok) {
    payload.workflow_tasks = Array.isArray(tasksResult.payload) ? tasksResult.payload : [];
    payload.workflow_tasks_loaded = tasksResult.response.ok;
    if (!tasksResult.response.ok) {
      payload.workflow_tasks_error = tasksResult.payload?.message || tasksResult.payload?.error || 'workflow tasks unavailable';
    }
  }
  return Response.json(payload, { status: response.status });
}
