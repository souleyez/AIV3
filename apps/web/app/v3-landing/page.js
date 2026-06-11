export const metadata = {
  title: 'DataMax V3',
  description: 'DataMax V3 企业级数据处理助手。',
};

export default function V3LandingPage() {
  return (
    <main style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #07111f 0%, #0b1d33 42%, #f2efe6 42%, #fffaf0 100%)',
      color: '#f8fafc',
      fontFamily: '"Avenir Next", "Segoe UI", sans-serif',
      overflow: 'hidden',
    }}>
      <section style={{
        display: 'grid',
        gridTemplateColumns: 'minmax(280px, 0.86fr) minmax(320px, 1.35fr)',
        gap: 32,
        alignItems: 'center',
        width: 'min(1320px, calc(100vw - 40px))',
        minHeight: '100vh',
        margin: '0 auto',
        padding: '44px 0',
      }}>
        <div style={{ display: 'grid', gap: 24 }}>
          <div style={{
            display: 'inline-flex',
            width: 'fit-content',
            border: '1px solid rgba(248, 250, 252, 0.22)',
            borderRadius: 999,
            padding: '8px 14px',
            color: '#d7e7ff',
            background: 'rgba(255, 255, 255, 0.08)',
            letterSpacing: 1,
          }}>
            DATAMAX V3
          </div>
          <h1 style={{
            margin: 0,
            fontSize: 'clamp(44px, 6vw, 86px)',
            lineHeight: 0.92,
            letterSpacing: '-0.07em',
            maxWidth: 680,
          }}>
            企业级数据处理助手
          </h1>
          <p style={{
            margin: 0,
            maxWidth: 520,
            color: '#cbd5e1',
            fontSize: 18,
            lineHeight: 1.75,
          }}>
            文档、数据库、网页和业务系统统一接入，自动生成问答、报表、HTML 产物和第三方回调能力。
          </p>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12 }}>
            <a href="/admin/login" style={{
              color: '#07111f',
              background: '#f8d36f',
              textDecoration: 'none',
              fontWeight: 800,
              borderRadius: 16,
              padding: '14px 20px',
              boxShadow: '0 18px 50px rgba(248, 211, 111, 0.28)',
            }}>
              管理台登录
            </a>
            <a href="/external-integrations/third-party-integration-api.zh-CN.html" style={{
              color: '#e5edf8',
              border: '1px solid rgba(229, 237, 248, 0.28)',
              textDecoration: 'none',
              fontWeight: 700,
              borderRadius: 16,
              padding: '14px 20px',
              background: 'rgba(255, 255, 255, 0.08)',
            }}>
              公开接口文档
            </a>
          </div>
        </div>
        <div style={{
          position: 'relative',
          borderRadius: 34,
          padding: 14,
          background: 'linear-gradient(135deg, rgba(255,255,255,0.9), rgba(248,211,111,0.78))',
          boxShadow: '0 34px 100px rgba(7, 17, 31, 0.34)',
        }}>
          <img
            src="/external-integrations/v3-enterprise-assistant-hero.png"
            alt="DataMax V3 enterprise assistant design"
            style={{
              display: 'block',
              width: '100%',
              height: 'auto',
              borderRadius: 24,
              background: '#fff',
            }}
          />
        </div>
      </section>
    </main>
  );
}
