import {
  codexControlAdminUrl,
  codexControlBaseUrl,
  codexControlBillingUrl,
  codexQuotaServiceToken,
  fetchCodexControlJson,
} from '../../../lib/codex-control-proxy';

export const dynamic = 'force-dynamic';

export async function GET(request) {
  const token = codexQuotaServiceToken();
  if (!token) {
    return Response.json({
      ok: true,
      configured: false,
      baseUrl: codexControlBaseUrl(),
      billingUrl: codexControlBillingUrl(),
      adminUrl: codexControlAdminUrl(),
      overview: null,
      message: 'V3_CODEX_QUOTA_SERVICE_TOKEN not configured',
    });
  }

  try {
    const incomingUrl = new URL(request.url);
    const search = incomingUrl.search || '?limit=20';
    const result = await fetchCodexControlJson('/api/codex/quotas/admin/overview', {
      search,
      serviceToken: true,
    });
    if (!result.ok) {
      return Response.json({
        ok: false,
        configured: true,
        error: 'codex_control_overview_unavailable',
        message: result.payload?.message || result.payload?.error?.code || 'codex-web 管理概览接口不可用',
        status: result.status,
      }, { status: 502 });
    }

    return Response.json({
      ok: true,
      configured: true,
      baseUrl: codexControlBaseUrl(),
      billingUrl: codexControlBillingUrl(),
      adminUrl: codexControlAdminUrl(),
      overview: result.payload?.overview || null,
    });
  } catch (error) {
    return Response.json({
      ok: false,
      configured: true,
      error: 'codex_control_unreachable',
      message: error instanceof Error ? error.message : 'codex-web 管理概览接口不可达',
    }, { status: 502 });
  }
}
