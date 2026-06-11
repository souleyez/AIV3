import { cookies } from 'next/headers';
import { NextResponse } from 'next/server';
import {
  ADMIN_CONSOLE_COOKIE,
  adminConsoleCookieOptions,
  adminConsoleCookieValue,
} from '../../../lib/admin-console-access';
import {
  ADMIN_MICROSOFT_STATE_COOKIE,
  adminMicrosoftConfig,
  decodeAdminMicrosoftStateCookie,
  exchangeMicrosoftCode,
  safeAdminNextPath,
  verifyMicrosoftIdToken,
} from '../../../lib/admin-microsoft-auth';

function redirectUrl(request, pathname, search = '') {
  const url = new URL(pathname, request.url);
  url.search = search;
  return url;
}

function loginRedirect(request, reason, next = '/admin') {
  return NextResponse.redirect(
    redirectUrl(request, '/admin/login', `?error=${encodeURIComponent(reason)}&next=${encodeURIComponent(safeAdminNextPath(next))}`),
    303,
  );
}

export async function GET(request) {
  const config = adminMicrosoftConfig(request.url);
  if (!config) {
    return loginRedirect(request, 'microsoft_config');
  }

  const error = request.nextUrl.searchParams.get('error');
  if (error) {
    return loginRedirect(request, 'microsoft_denied');
  }

  const cookieStore = await cookies();
  const stateCookie = cookieStore.get(ADMIN_MICROSOFT_STATE_COOKIE)?.value;
  const state = decodeAdminMicrosoftStateCookie(stateCookie, config);
  if (!state || request.nextUrl.searchParams.get('state') !== state.state) {
    return loginRedirect(request, 'microsoft_state');
  }

  const code = String(request.nextUrl.searchParams.get('code') || '').trim();
  if (!code) {
    return loginRedirect(request, 'microsoft_code', state.next);
  }

  try {
    const tokenPayload = await exchangeMicrosoftCode({ code, config });
    await verifyMicrosoftIdToken(tokenPayload.id_token, config, state.nonce);

    const response = NextResponse.redirect(redirectUrl(request, state.next), 303);
    response.cookies.delete(ADMIN_MICROSOFT_STATE_COOKIE);
    response.cookies.set(
      ADMIN_CONSOLE_COOKIE,
      adminConsoleCookieValue(),
      adminConsoleCookieOptions({ secure: request.nextUrl.protocol === 'https:' }),
    );
    return response;
  } catch (error) {
    console.warn('admin microsoft auth failed', error instanceof Error ? error.message : error);
    const response = loginRedirect(request, 'microsoft_failed', state.next);
    response.cookies.delete(ADMIN_MICROSOFT_STATE_COOKIE);
    return response;
  }
}
