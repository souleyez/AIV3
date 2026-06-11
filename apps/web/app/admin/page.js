import { cookies } from 'next/headers';
import { redirect } from 'next/navigation';
import ExternalIntegrationsPageClient from '../external-integrations/ExternalIntegrationsPageClient';
import { ADMIN_CONSOLE_COOKIE, hasAdminConsoleAccessCookieValue } from '../lib/admin-console-access';
import AdminShell from './AdminShell';

export const metadata = {
  title: 'DataMax 管理台',
  description: 'DataMax 外部集成、运营观测和公开接口管理。',
};

export default async function AdminConsolePage() {
  const cookieStore = await cookies();
  const accessCookie = cookieStore.get(ADMIN_CONSOLE_COOKIE)?.value;
  if (!hasAdminConsoleAccessCookieValue(accessCookie)) {
    redirect('/admin/login?next=/admin');
  }
  return (
    <AdminShell active="overview">
      <ExternalIntegrationsPageClient />
    </AdminShell>
  );
}
