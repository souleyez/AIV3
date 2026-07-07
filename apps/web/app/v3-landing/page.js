export const metadata = {
  title: 'DataMax V3',
  description: 'DataMax V3 企业级数据助手。',
};

const DOWNLOAD_LINKS = [
  {
    title: 'V3企业定制agent终端 Windows',
    href: '/downloads/codex/V3企业定制agent终端-Windows.zip',
    text: 'Windows 版，内置 Codex runtime。',
  },
  {
    title: 'V3企业定制agent终端 macOS',
    href: '/downloads/codex/V3企业定制agent终端-macOS.zip',
    text: 'macOS 版，内置 Codex runtime。',
  },
];

export default function V3LandingPage() {
  return (
    <main style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #07111f 0%, #0b1d33 42%, #f2efe6 42%, #fffaf0 100%)',
      color: '#f8fafc',
      fontFamily: '"Avenir Next", "Segoe UI", sans-serif',
      overflowX: 'hidden',
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
            letterSpacing: 0,
            maxWidth: 680,
          }}>
            企业级数据助手
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
            <a href="#codex-downloads" style={{
              color: '#07111f',
              background: '#f8d36f',
              textDecoration: 'none',
              fontWeight: 800,
              borderRadius: 16,
              padding: '14px 20px',
              boxShadow: '0 18px 50px rgba(248, 211, 111, 0.28)',
            }}>
              V3企业定制agent终端
            </a>
            <a href="/admin/login" style={{
              color: '#e5edf8',
              border: '1px solid rgba(229, 237, 248, 0.28)',
              textDecoration: 'none',
              fontWeight: 800,
              borderRadius: 16,
              padding: '14px 20px',
              background: 'rgba(255, 255, 255, 0.08)',
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
            <a href="/external-integrations/datamax-v3-enterprise-solution.html" style={{
              color: '#082033',
              border: '1px solid rgba(8, 32, 51, 0.14)',
              textDecoration: 'none',
              fontWeight: 800,
              borderRadius: 16,
              padding: '14px 20px',
              background: 'rgba(255, 250, 240, 0.92)',
              boxShadow: '0 18px 46px rgba(7, 17, 31, 0.16)',
            }}>
              企业公开方案
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
      <section id="codex-downloads" style={{
        width: 'min(1320px, calc(100vw - 40px))',
        margin: '0 auto',
        padding: '0 0 56px',
      }}>
        <div style={{
          display: 'grid',
          gap: 18,
          padding: 24,
          borderRadius: 28,
          border: '1px solid rgba(8, 32, 51, 0.12)',
          background: 'rgba(255, 250, 240, 0.94)',
          color: '#0b1d33',
          boxShadow: '0 28px 80px rgba(7, 17, 31, 0.2)',
        }}>
          <div style={{
            display: 'grid',
            gap: 8,
            maxWidth: 820,
          }}>
            <span style={{
              color: '#0f766e',
              fontSize: 13,
              fontWeight: 900,
              letterSpacing: 0,
            }}>
              V3 AGENT TERMINAL
            </span>
            <h2 style={{
              margin: 0,
              color: '#07111f',
              fontSize: 'clamp(28px, 4vw, 48px)',
              lineHeight: 1.08,
              letterSpacing: 0,
            }}>
              V3企业定制agent终端
            </h2>
            <p style={{
              margin: 0,
              color: '#415168',
              fontSize: 16,
              fontWeight: 700,
              lineHeight: 1.7,
            }}>
              下载一个客户端包即可完成安装试用，Codex runtime 已随包内置。
            </p>
          </div>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: 12,
          }}>
            {DOWNLOAD_LINKS.map((item) => (
              <a
                href={item.href}
                key={item.href}
                style={{
                  minWidth: 0,
                  display: 'grid',
                  gap: 8,
                  padding: 18,
                  border: '1px solid rgba(15, 118, 110, 0.18)',
                  borderRadius: 18,
                  background: '#ffffff',
                  color: '#0b1d33',
                  textDecoration: 'none',
                }}
              >
                <strong style={{
                  fontSize: 18,
                  lineHeight: 1.35,
                }}>
                  {item.title}
                </strong>
                <span style={{
                  color: '#607084',
                  fontSize: 13,
                  fontWeight: 700,
                  lineHeight: 1.6,
                }}>
                  {item.text}
                </span>
              </a>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}
