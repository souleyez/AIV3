import { cookies } from 'next/headers';
import { redirect } from 'next/navigation';
import { ADMIN_CONSOLE_COOKIE, adminConsoleAccessRequired, hasAdminConsoleAccessCookieValue } from '../../lib/admin-console-access';
import { adminMicrosoftAuthEnabled } from '../../lib/admin-microsoft-auth';

export const metadata = {
  title: 'DataMax V3 管理登录',
  description: '登录 DataMax V3 管理台。',
};

export default async function AdminLoginPage({ searchParams }) {
  const params = await Promise.resolve(searchParams || {});
  const next = typeof params.next === 'string' && params.next.startsWith('/admin') ? params.next : '/admin';
  const error = typeof params.error === 'string' ? params.error : '';
  const errorMessage = {
    1: '密钥不正确，请重新输入。',
    microsoft_config: 'Microsoft 登录未完成配置，请先检查管理台认证环境变量。',
    microsoft_denied: 'Microsoft 登录已取消或未授权。',
    microsoft_state: 'Microsoft 登录状态已失效，请重新发起登录。',
    microsoft_code: 'Microsoft 登录未返回授权码，请重新发起登录。',
    microsoft_failed: 'Microsoft 登录校验失败，请确认账号在管理台白名单内。',
  }[error] || '';
  const microsoftEnabled = adminMicrosoftAuthEnabled();
  const cookieStore = await cookies();
  const accessCookie = cookieStore.get(ADMIN_CONSOLE_COOKIE)?.value;
  if (hasAdminConsoleAccessCookieValue(accessCookie)) {
    redirect(next);
  }

  return (
    <main style={{
      minHeight: '100vh',
      display: 'grid',
      placeItems: 'center',
      padding: 24,
      background: 'radial-gradient(circle at top left, #1f3a5b 0, #08111f 42%, #030712 100%)',
      color: '#f8fafc',
      fontFamily: '"Avenir Next", "Segoe UI", sans-serif',
    }}>
      <section style={{
        width: 'min(440px, 100%)',
        border: '1px solid rgba(226, 232, 240, 0.16)',
        borderRadius: 28,
        padding: 28,
        background: 'rgba(15, 23, 42, 0.82)',
        boxShadow: '0 28px 100px rgba(0, 0, 0, 0.34)',
      }}>
        <a href="/" style={{ color: '#93c5fd', textDecoration: 'none', fontWeight: 700 }}>返回首页</a>
        <h1 style={{ margin: '22px 0 8px', fontSize: 34, letterSpacing: '-0.04em' }}>管理台登录</h1>
        <p style={{ margin: '0 0 24px', color: '#cbd5e1', lineHeight: 1.7 }}>
          输入管理访问密钥或使用 Microsoft 账号进入 DataMax 管理台。公开接口文档仍可直接访问。
        </p>
        {!adminConsoleAccessRequired() ? (
          <div style={{
            marginBottom: 16,
            border: '1px solid rgba(248, 211, 111, 0.36)',
            borderRadius: 14,
            padding: 12,
            color: '#fde68a',
            background: 'rgba(248, 211, 111, 0.08)',
          }}>
            当前环境未配置管理密钥，本地环境会直接放行；生产建议配置 ADMIN_CONSOLE_ACCESS_KEY。
          </div>
        ) : null}
        {errorMessage ? (
          <div style={{
            marginBottom: 16,
            border: '1px solid rgba(248, 113, 113, 0.34)',
            borderRadius: 14,
            padding: 12,
            color: '#fecaca',
            background: 'rgba(127, 29, 29, 0.22)',
          }}>
            {errorMessage}
          </div>
        ) : null}
        {microsoftEnabled ? (
          <a
            href={`/admin/microsoft/start?next=${encodeURIComponent(next)}`}
            style={{
              display: 'block',
              marginBottom: 14,
              border: '1px solid rgba(147, 197, 253, 0.34)',
              borderRadius: 16,
              padding: '13px 16px',
              color: '#e0f2fe',
              background: 'rgba(37, 99, 235, 0.22)',
              fontWeight: 900,
              textAlign: 'center',
              textDecoration: 'none',
            }}
          >
            使用 Microsoft 登录
          </a>
        ) : null}
        <form action="/admin/access" method="post" style={{ display: 'grid', gap: 14 }}>
          <input type="hidden" name="next" value={next} />
          <label style={{ display: 'grid', gap: 8, color: '#dbeafe', fontWeight: 700 }}>
            <span>访问密钥</span>
            <input
              name="access_key"
              type="password"
              autoComplete="current-password"
              required={adminConsoleAccessRequired()}
              style={{
                border: '1px solid rgba(226, 232, 240, 0.22)',
                borderRadius: 14,
                padding: '14px 15px',
                background: 'rgba(255, 255, 255, 0.08)',
                color: '#fff',
                outline: 'none',
              }}
            />
          </label>
          <button type="submit" style={{
            border: 0,
            borderRadius: 16,
            padding: '14px 18px',
            color: '#07111f',
            background: '#f8d36f',
            fontWeight: 900,
            cursor: 'pointer',
          }}>
            进入管理台
          </button>
        </form>
      </section>
    </main>
  );
}
