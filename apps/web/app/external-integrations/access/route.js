import { NextResponse } from 'next/server';
import {
  EXTERNAL_OBSERVABILITY_COOKIE,
  externalObservabilityCookieOptions,
  externalObservabilityCookieValue,
  verifyExternalObservabilityKey,
} from '../../lib/external-observability-access';

function externalIntegrationsUrl(request, search = '') {
  const url = new URL('/external-integrations', request.url);
  url.search = search;
  return url;
}

export async function GET(request) {
  return NextResponse.redirect(externalIntegrationsUrl(request));
}

export async function POST(request) {
  const form = await request.formData();
  const accessKey = form.get('access_key');
  if (!verifyExternalObservabilityKey(accessKey)) {
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
