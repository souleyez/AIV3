import './globals.css';

export const metadata = {
  title: 'DataMax企业级数据助手',
  description: '无需开发对接，文档数据库爬虫采集皆可入库，秒生数据可视化报表，支持移动端。',
};

export const viewport = {
  width: 'device-width',
  initialScale: 1,
  viewportFit: 'cover',
};

export const dynamic = 'force-dynamic';
export const revalidate = 0;

export default function RootLayout({ children }) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
