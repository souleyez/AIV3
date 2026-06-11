import { cookies } from 'next/headers';
import { redirect } from 'next/navigation';
import { ADMIN_CONSOLE_COOKIE, adminConsoleAccessRequired, hasAdminConsoleAccessCookieValue } from '../../lib/admin-console-access';
import { adminMicrosoftAuthEnabled } from '../../lib/admin-microsoft-auth';

export const metadata = {
  title: 'DataMax 管理登录',
  description: '登录 DataMax 管理台。',
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
    <main className="external-observability-shell admin-login-shell">
      <section className="external-product-hero admin-login-hero">
        <div className="external-product-copy admin-login-copy">
          <p className="external-kicker">DataMax Admin Console</p>
          <h1>
            <span>管理台</span>
            <span>安全登录</span>
          </h1>
          <p className="external-product-tagline">
            <span>统一进入运营观测、外部集成、模型与工作区管理。</span>
            <span>管理动作受鉴权保护，公开接口文档仍可直接访问。</span>
          </p>
          <div className="external-contact-row" aria-label="管理台入口">
            <a href="/">返回管理首页</a>
            <a href="/external-integrations/third-party-integration-api.zh-CN.html">公开接口文档</a>
            <span>Microsoft SSO</span>
            <span>访问密钥</span>
          </div>
          <div className="external-feature-chips" aria-label="管理台能力">
            <span>接入观测</span>
            <span>执行器队列</span>
            <span>文档诊断</span>
            <span>工作区管理</span>
          </div>
          <figure className="admin-login-preview">
            <img
              src="/external-integrations/v3-enterprise-assistant-hero.png"
              alt="DataMax 管理台视觉参考"
            />
          </figure>
        </div>

        <section className="admin-login-card" aria-label="管理台登录">
          <div className="admin-login-card-head">
            <p className="external-kicker">Restricted Area</p>
            <h2>进入管理台</h2>
            <p>
              输入管理访问密钥，或使用已配置白名单的 Microsoft 账号登录。
            </p>
          </div>
        {!adminConsoleAccessRequired() ? (
          <div className="admin-login-alert admin-login-alert-warning">
            当前环境未配置管理密钥，本地环境会直接放行；生产建议配置 ADMIN_CONSOLE_ACCESS_KEY。
          </div>
        ) : null}
        {errorMessage ? (
          <div className="admin-login-alert admin-login-alert-error">
            {errorMessage}
          </div>
        ) : null}
        {microsoftEnabled ? (
          <a
            href={`/admin/microsoft/start?next=${encodeURIComponent(next)}`}
            className="admin-login-sso"
          >
            使用 Microsoft 登录
          </a>
        ) : null}
        <form action="/admin/access" method="post" className="admin-login-form">
          <input type="hidden" name="next" value={next} />
          <label>
            <span>访问密钥</span>
            <input
              name="access_key"
              type="password"
              autoComplete="current-password"
              required={adminConsoleAccessRequired()}
              placeholder="输入管理访问密钥"
            />
          </label>
          <button type="submit">
            进入管理台
          </button>
        </form>
        <p className="admin-login-footnote">
          未授权用户不会进入后台页面；公共文档和第三方回调接口不依赖此登录态。
        </p>
      </section>
      </section>
    </main>
  );
}
