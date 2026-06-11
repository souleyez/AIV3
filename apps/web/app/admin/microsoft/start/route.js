import { NextResponse } from 'next/server';
import {
  ADMIN_MICROSOFT_STATE_COOKIE,
  adminMicrosoftCookieOptions,
  buildAdminMicrosoftAuthorizeUrl,
  safeAdminNextPath,
} from '../../../lib/admin-microsoft-auth';

function redirectUrl(request, pathname, search = '') {
  const url = new URL(pathname, request.url);
  url.search = search;
  return url;
}

export async function GET(request) {
  const next = safeAdminNextPath(request.nextUrl.searchParams.get('next'));
  const auth = buildAdminMicrosoftAuthorizeUrl({ requestUrl: request.url, next });
  if (!auth) {
    return NextResponse.redirect(
      redirectUrl(request, '/admin/login', `?error=microsoft_config&next=${encodeURIComponent(next)}`),
      303,
    );
  }

  const response = NextResponse.redirect(auth.url, 303);
  response.cookies.set(
    ADMIN_MICROSOFT_STATE_COOKIE,
    auth.cookieValue,
    adminMicrosoftCookieOptions({ secure: request.nextUrl.protocol === 'https:' }),
  );
  return response;
}
