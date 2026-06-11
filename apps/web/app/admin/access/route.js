import { NextResponse } from 'next/server';
import {
  ADMIN_CONSOLE_COOKIE,
  adminConsoleAccessRequired,
  adminConsoleCookieOptions,
  adminConsoleCookieValue,
  verifyAdminConsoleAccessKey,
} from '../../lib/admin-console-access';
import { ADMIN_MICROSOFT_STATE_COOKIE } from '../../lib/admin-microsoft-auth';

function safeNextPath(value) {
  const next = String(value || '').trim();
  return next.startsWith('/admin') ? next : '/admin';
}

function redirectUrl(request, pathname, search = '') {
  const url = new URL(pathname, request.url);
  url.search = search;
  return url;
}

export async function POST(request) {
  const form = await request.formData();
  const next = safeNextPath(form.get('next'));
  const accessKey = form.get('access_key');
  const action = String(form.get('action') || 'login');

  if (action === 'logout') {
    const response = NextResponse.redirect(redirectUrl(request, '/'), 303);
    response.cookies.delete(ADMIN_CONSOLE_COOKIE);
    response.cookies.delete(ADMIN_MICROSOFT_STATE_COOKIE);
    return response;
  }

  if (adminConsoleAccessRequired() && !verifyAdminConsoleAccessKey(accessKey)) {
    return NextResponse.redirect(
      redirectUrl(request, '/admin/login', `?error=1&next=${encodeURIComponent(next)}`),
      303,
    );
  }

  const response = NextResponse.redirect(redirectUrl(request, next), 303);
  response.cookies.set(
    ADMIN_CONSOLE_COOKIE,
    adminConsoleCookieValue(),
    adminConsoleCookieOptions({ secure: request.nextUrl.protocol === 'https:' }),
  );
  return response;
}
