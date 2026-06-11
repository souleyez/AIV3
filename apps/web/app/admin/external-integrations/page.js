import { cookies } from 'next/headers';
import { redirect } from 'next/navigation';
import ExternalIntegrationsPageClient from '../../external-integrations/ExternalIntegrationsPageClient';
import { ADMIN_CONSOLE_COOKIE, hasAdminConsoleAccessCookieValue } from '../../lib/admin-console-access';
import AdminShell from '../AdminShell';

export const metadata = {
  title: 'DataMax V3 外部集成管理',
  description: 'DataMax V3 第三方通道、观测和公开接口管理。',
};

export default async function AdminExternalIntegrationsPage() {
  const cookieStore = await cookies();
  const accessCookie = cookieStore.get(ADMIN_CONSOLE_COOKIE)?.value;
  if (!hasAdminConsoleAccessCookieValue(accessCookie)) {
    redirect('/admin/login?next=/admin/external-integrations');
  }
  return (
    <AdminShell active="external">
      <ExternalIntegrationsPageClient />
    </AdminShell>
  );
}
