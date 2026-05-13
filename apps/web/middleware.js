import { NextResponse } from 'next/server';

const EXTERNAL_OBSERVABILITY_HOSTS = new Set([
  'v3.elepcloud.com',
]);

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
  if (!EXTERNAL_OBSERVABILITY_HOSTS.has(hostWithoutPort(request))) {
    return NextResponse.next();
  }

  const url = request.nextUrl.clone();
  if (url.pathname === '/') {
    url.pathname = '/external-integrations';
    return NextResponse.rewrite(url);
  }

  if (isExternalObservationAllowedPath(url.pathname)) {
    return NextResponse.next();
  }

  url.pathname = '/external-integrations';
  url.search = '';
  return NextResponse.redirect(url);
}

export const config = {
  matcher: ['/((?!.*\\..*).*)', '/favicon.ico', '/robots.txt', '/sitemap.xml'],
};
