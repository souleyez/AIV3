import { cookies } from 'next/headers';
import { redirect } from 'next/navigation';
import HomePageClient from '../../HomePageClient';
import { ADMIN_CONSOLE_COOKIE, hasAdminConsoleAccessCookieValue } from '../../lib/admin-console-access';
import AdminShell from '../AdminShell';

export const metadata = {
  title: 'DataMax 工作区',
  description: 'DataMax 主站工作区管理入口。',
};

export default async function AdminWorkspacePage() {
  const cookieStore = await cookies();
  const accessCookie = cookieStore.get(ADMIN_CONSOLE_COOKIE)?.value;
  if (!hasAdminConsoleAccessCookieValue(accessCookie)) {
    redirect('/admin/login?next=/admin/workspace');
  }
  return (
    <AdminShell active="workspace">
      <HomePageClient />
    </AdminShell>
  );
}
