import { cookies } from 'next/headers';
import { redirect } from 'next/navigation';
import HomePageClient from '../HomePageClient';
import { ADMIN_CONSOLE_COOKIE, hasAdminConsoleAccessCookieValue } from '../lib/admin-console-access';
import AdminShell from './AdminShell';

export const metadata = {
  title: 'DataMax V3 管理台',
  description: 'DataMax V3 后台管理台。',
};

export default async function AdminConsolePage() {
  const cookieStore = await cookies();
  const accessCookie = cookieStore.get(ADMIN_CONSOLE_COOKIE)?.value;
  if (!hasAdminConsoleAccessCookieValue(accessCookie)) {
    redirect('/admin/login?next=/admin');
  }
  return (
    <AdminShell active="workspace">
      <HomePageClient />
    </AdminShell>
  );
}
