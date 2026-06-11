export default function AdminShell({ active = 'workspace', children }) {
  const links = [
    { key: 'workspace', href: '/admin', label: '工作台' },
    { key: 'external', href: '/admin/external-integrations', label: '外部集成' },
    { key: 'api', href: '/external-integrations/third-party-integration-api.zh-CN.html', label: '公开接口文档' },
  ];

  return (
    <>
      <div style={{
        position: 'fixed',
        top: 14,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
        display: 'flex',
        alignItems: 'center',
        gap: 10,
        maxWidth: 'calc(100vw - 28px)',
        border: '1px solid rgba(148, 163, 184, 0.26)',
        borderRadius: 999,
        padding: '8px 10px 8px 14px',
        background: 'rgba(8, 17, 31, 0.78)',
        boxShadow: '0 18px 54px rgba(2, 6, 23, 0.32)',
        backdropFilter: 'blur(18px)',
        color: '#e5edf8',
        fontFamily: '"Avenir Next", "Segoe UI", sans-serif',
      }}>
        <a href="/" style={{
          color: '#f8d36f',
          textDecoration: 'none',
          fontWeight: 900,
          whiteSpace: 'nowrap',
          padding: '7px 8px',
        }}>
          DataMax V3
        </a>
        <nav style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          {links.map((link) => (
            <a
              key={link.key}
              href={link.href}
              style={{
                color: active === link.key ? '#07111f' : '#dbeafe',
                background: active === link.key ? '#f8d36f' : 'transparent',
                textDecoration: 'none',
                fontSize: 13,
                fontWeight: 800,
                borderRadius: 999,
                padding: '8px 11px',
                whiteSpace: 'nowrap',
              }}
            >
              {link.label}
            </a>
          ))}
        </nav>
        <form action="/admin/access" method="post" style={{ margin: 0 }}>
          <input type="hidden" name="action" value="logout" />
          <button type="submit" style={{
            border: '1px solid rgba(226, 232, 240, 0.18)',
            borderRadius: 999,
            padding: '8px 11px',
            color: '#fecaca',
            background: 'rgba(127, 29, 29, 0.18)',
            fontSize: 13,
            fontWeight: 800,
            cursor: 'pointer',
            whiteSpace: 'nowrap',
          }}>
            退出
          </button>
        </form>
      </div>
      {children}
    </>
  );
}
