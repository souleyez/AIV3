import './globals.css';

export const metadata = {
  title: 'AI Data Platform V3 | 智能助手',
  description: 'Rust V3 平台的智能助手工作台，支持数据集选择、会话回看与发布结果浏览。',
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
