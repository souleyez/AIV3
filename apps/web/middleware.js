import { NextResponse } from 'next/server';

const ADMIN_CONSOLE_HOSTS = new Set([
  'v3.elepcloud.com',
]);

function hostWithoutPort(request) {
  return (request.headers.get('host') || '').split(':')[0].toLowerCase();
}

function isExternalObservationAllowedPath(pathname) {
  return pathname.startsWith('/v1/')
    || pathname.startsWith('/api/v3/')
    || pathname.startsWith('/_next/')
    || pathname === '/favicon.ico'
    || pathname === '/robots.txt'
    || pathname === '/sitemap.xml';
}

export function middleware(request) {
  if (!ADMIN_CONSOLE_HOSTS.has(hostWithoutPort(request))) {
    return NextResponse.next();
  }

  const url = request.nextUrl.clone();
  if (url.pathname === '/') {
    url.pathname = '/admin';
    return NextResponse.redirect(url);
  }

  if (
    url.pathname.startsWith('/admin')
    || url.pathname === '/external-integrations/access'
    || isExternalObservationAllowedPath(url.pathname)
  ) {
    return NextResponse.next();
  }

  if (url.pathname === '/external-integrations') {
    url.pathname = '/admin/external-integrations';
    return NextResponse.redirect(url);
  }

  url.pathname = '/';
  url.search = '';
  return NextResponse.redirect(url);
}

export const config = {
  matcher: ['/((?!.*\\..*).*)', '/favicon.ico', '/robots.txt', '/sitemap.xml'],
};
