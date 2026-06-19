import {
  codexControlAdminUrl,
  codexControlBaseUrl,
  codexControlBillingUrl,
  fetchCodexControlJson,
} from '../../../lib/codex-control-proxy';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const result = await fetchCodexControlJson('/api/codex/quotas/plans');
    if (!result.ok) {
      return Response.json({
        ok: false,
        error: 'codex_control_plans_unavailable',
        message: result.payload?.message || result.payload?.error?.code || 'codex-web 套餐接口不可用',
        status: result.status,
      }, { status: 502 });
    }

    return Response.json({
      ok: true,
      configured: true,
      baseUrl: codexControlBaseUrl(),
      billingUrl: codexControlBillingUrl(),
      adminUrl: codexControlAdminUrl(),
      plans: Array.isArray(result.payload?.plans) ? result.payload.plans : [],
    });
  } catch (error) {
    return Response.json({
      ok: false,
      error: 'codex_control_unreachable',
      message: error instanceof Error ? error.message : 'codex-web 套餐接口不可达',
    }, { status: 502 });
  }
}
