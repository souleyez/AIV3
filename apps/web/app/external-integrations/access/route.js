import { NextResponse } from 'next/server';
import {
  EXTERNAL_OBSERVABILITY_COOKIE,
  consumeExternalObservabilityAccessKey,
  externalObservabilityCookieOptions,
  externalObservabilityCookieValue,
} from '../../lib/external-observability-access';

function firstForwardedHeaderValue(value) {
  return String(value || '').split(',')[0].trim();
}

function externalIntegrationsOrigin(request) {
  const forwardedHost = firstForwardedHeaderValue(request.headers.get('x-forwarded-host'));
  const forwardedProto = firstForwardedHeaderValue(request.headers.get('x-forwarded-proto'));
  const host = forwardedHost || request.headers.get('host');
  if (host) {
    const proto = forwardedProto || request.nextUrl.protocol.replace(':', '') || 'https';
    return `${proto}://${host}`;
  }
  return request.url;
}

function externalIntegrationsUrl(request, search = '') {
  const url = new URL('/external-integrations', externalIntegrationsOrigin(request));
  url.search = search;
  return url;
}

export async function GET(request) {
  return NextResponse.redirect(externalIntegrationsUrl(request));
}

export async function POST(request) {
  const form = await request.formData();
  const accessKey = form.get('access_key');
  if (!consumeExternalObservabilityAccessKey(accessKey)) {
    return NextResponse.redirect(externalIntegrationsUrl(request, '?conversation_tests=1&access_error=1'), 303);
  }
  const response = NextResponse.redirect(externalIntegrationsUrl(request, '?conversation_tests=1'), 303);
  response.cookies.set(
    EXTERNAL_OBSERVABILITY_COOKIE,
    externalObservabilityCookieValue(),
    externalObservabilityCookieOptions({ secure: request.nextUrl.protocol === 'https:' }),
  );
  return response;
}
