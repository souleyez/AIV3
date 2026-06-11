import { NextResponse } from 'next/server';

const ADMIN_CONSOLE_HOSTS = new Set([
  'v3.elepcloud.com',
]);

const PUBLIC_SITE_HOSTS = new Set([
  'doc.elepcloud.com',
]);

const PRIMARY_ADMIN_CONSOLE_ORIGIN = 'https://v3.elepcloud.com';

function hostWithoutPort(request) {
  return (request.headers.get('host') || '').split(':')[0].toLowerCase();
}

function isExternalObservationAllowedPath(pathname) {
  return pathname === '/external-integrations'
    || pathname.startsWith('/external-integrations/')
    || pathname.startsWith('/v1/')
    || pathname.startsWith('/api/v3/')
    || pathname.startsWith('/_next/')
    || pathname === '/favicon.ico'
    || pathname === '/robots.txt'
    || pathname === '/sitemap.xml';
}

export function middleware(request) {
  const host = hostWithoutPort(request);
  const url = request.nextUrl.clone();
  if (!ADMIN_CONSOLE_HOSTS.has(host)) {
    if (PUBLIC_SITE_HOSTS.has(host) && url.pathname.startsWith('/admin')) {
      return NextResponse.redirect(new URL(`${url.pathname}${url.search}`, PRIMARY_ADMIN_CONSOLE_ORIGIN));
    }
    return NextResponse.next();
  }

  if (url.pathname === '/') {
    url.pathname = '/v3-landing';
    return NextResponse.rewrite(url);
  }

  if (
    url.pathname.startsWith('/admin')
    || url.pathname === '/external-integrations/access'
    || isExternalObservationAllowedPath(url.pathname)
  ) {
    return NextResponse.next();
  }

  url.pathname = '/';
  url.search = '';
  return NextResponse.redirect(url);
}

export const config = {
  matcher: ['/((?!.*\\..*).*)', '/favicon.ico', '/robots.txt', '/sitemap.xml'],
};
